use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum SpaceCommands {
    /// List spaces in workspace
    List {
        /// Include archived spaces
        #[arg(long)]
        archived: bool,
    },
    /// Get space details
    Get {
        /// Space ID
        id: String,
    },
    /// Create a new space
    Create {
        /// Space name
        #[arg(long)]
        name: String,
        /// Make space private
        #[arg(long)]
        private: bool,
        /// Allow multiple assignees
        #[arg(long)]
        multiple_assignees: bool,
    },
    /// Update a space
    Update {
        /// Space ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Color hex
        #[arg(long)]
        color: Option<String>,
    },
    /// Delete a space
    Delete {
        /// Space ID
        id: String,
    },
}

pub async fn execute(command: SpaceCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        SpaceCommands::List { archived } => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/space?archived={}", ws_id, archived))
                .await?;
            let spaces = resp
                .get("spaces")
                .and_then(|s| s.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&spaces, &["id", "name", "private", "archived"], "id");
            Ok(())
        }
        SpaceCommands::Get { id } => {
            let resp = client.get(&format!("/v2/space/{}", id)).await?;
            output.print_single(&resp, &["id", "name", "private", "archived"], "id");
            Ok(())
        }
        SpaceCommands::Create {
            name,
            private,
            multiple_assignees,
        } => {
            let ws_id = resolve_workspace(cli)?;
            let body = serde_json::json!({
                "name": name,
                "multiple_assignees": multiple_assignees,
                "features": {
                    "due_dates": { "enabled": true },
                    "priorities": { "enabled": true },
                    "tags": { "enabled": true },
                    "time_estimates": { "enabled": true },
                },
                "private": private,
            });
            let resp = client
                .post(&format!("/v2/team/{}/space", ws_id), &body)
                .await?;
            output.print_single(&resp, &["id", "name", "private"], "id");
            Ok(())
        }
        SpaceCommands::Update { id, name, color } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(c) = color {
                body.insert("color".into(), serde_json::Value::String(c));
            }
            let resp = client
                .put(&format!("/v2/space/{}", id), &serde_json::Value::Object(body))
                .await?;
            output.print_single(&resp, &["id", "name", "private"], "id");
            Ok(())
        }
        SpaceCommands::Delete { id } => {
            client.delete(&format!("/v2/space/{}", id)).await?;
            output.print_message(&format!("Space {} deleted", id));
            Ok(())
        }
    }
}
