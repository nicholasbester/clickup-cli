use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum TaskTypeCommands {
    /// List custom task types in the workspace
    List,
}

const TASK_TYPE_FIELDS: &[&str] = &["id", "name", "name_plural", "description"];

pub async fn execute(command: TaskTypeCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        TaskTypeCommands::List => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/custom_item", ws_id))
                .await?;
            let items = resp
                .get("custom_items")
                .and_then(|i| i.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&items, TASK_TYPE_FIELDS, "id");
            Ok(())
        }
    }
}
