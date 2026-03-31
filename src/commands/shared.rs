use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum SharedCommands {
    /// List shared hierarchy (tasks, lists, folders) for the workspace
    List,
}

pub async fn execute(command: SharedCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        SharedCommands::List => {
            let team_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/shared", team_id))
                .await?;

            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                return Ok(());
            }

            let shared = resp.get("shared").cloned().unwrap_or(resp);
            let tasks = shared
                .get("tasks")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let lists = shared
                .get("lists")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let folders = shared
                .get("folders")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);

            let summary = vec![serde_json::json!({
                "tasks": tasks,
                "lists": lists,
                "folders": folders,
            })];
            output.print_items(&summary, &["tasks", "lists", "folders"], "tasks");
            Ok(())
        }
    }
}
