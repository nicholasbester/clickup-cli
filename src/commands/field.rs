use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum FieldCommands {
    /// List custom fields
    List {
        /// List ID
        #[arg(long)]
        list: Option<String>,
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID
        #[arg(long)]
        space: Option<String>,
        /// Workspace-level fields
        #[arg(long)]
        workspace: bool,
    },
    /// Set a custom field value on a task
    Set {
        /// Task ID
        task_id: String,
        /// Field ID
        field_id: String,
        /// Field value (string, number, or JSON)
        #[arg(long)]
        value: String,
    },
    /// Unset (clear) a custom field value on a task
    Unset {
        /// Task ID
        task_id: String,
        /// Field ID
        field_id: String,
    },
}

const FIELD_FIELDS: &[&str] = &["id", "name", "type", "required"];

pub async fn execute(command: FieldCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        FieldCommands::List {
            list,
            folder,
            space,
            workspace,
        } => {
            let path = if let Some(list_id) = list {
                format!("/v2/list/{}/field", list_id)
            } else if let Some(folder_id) = folder {
                format!("/v2/folder/{}/field", folder_id)
            } else if let Some(space_id) = space {
                format!("/v2/space/{}/field", space_id)
            } else if workspace {
                let ws_id = resolve_workspace(cli)?;
                format!("/v2/team/{}/field", ws_id)
            } else {
                return Err(CliError::ClientError {
                    message: "Specify --list, --folder, --space, or --workspace".into(),
                    status: 0,
                });
            };

            let resp = client.get(&path).await?;
            let fields = resp
                .get("fields")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&fields, FIELD_FIELDS, "id");
            Ok(())
        }
        FieldCommands::Set {
            task_id,
            field_id,
            value,
        } => {
            // Try to parse value as JSON first, fallback to string
            let parsed_value: serde_json::Value = serde_json::from_str(&value)
                .unwrap_or_else(|_| serde_json::Value::String(value));
            let body = serde_json::json!({ "value": parsed_value });
            let resp = client
                .post(&format!("/v2/task/{}/field/{}", task_id, field_id), &body)
                .await?;
            output.print_single(&resp, FIELD_FIELDS, "id");
            Ok(())
        }
        FieldCommands::Unset { task_id, field_id } => {
            client
                .delete(&format!("/v2/task/{}/field/{}", task_id, field_id))
                .await?;
            output.print_message(&format!(
                "Field {} cleared on task {}",
                field_id, task_id
            ));
            Ok(())
        }
    }
}
