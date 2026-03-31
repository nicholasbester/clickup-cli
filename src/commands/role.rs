use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum RoleCommands {
    /// List custom roles in the workspace (Enterprise only)
    List,
}

const ROLE_FIELDS: &[&str] = &["id", "name", "custom"];

pub async fn execute(command: RoleCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        RoleCommands::List => {
            let team_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/customroles", team_id))
                .await?;
            let roles = resp
                .get("custom_roles")
                .and_then(|r| r.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&roles, ROLE_FIELDS, "id");
            Ok(())
        }
    }
}
