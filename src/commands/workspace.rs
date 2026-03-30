use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::config::Config;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// List workspaces
    List,
    /// Show seat usage
    Seats,
    /// Show current plan
    Plan,
}

pub fn resolve_workspace(cli: &Cli) -> Result<String, CliError> {
    if let Some(ws) = &cli.workspace {
        return Ok(ws.clone());
    }
    let config = Config::load()?;
    config
        .defaults
        .workspace_id
        .ok_or_else(|| CliError::ConfigError("No default workspace. Use --workspace or run 'clickup setup'".into()))
}

pub async fn execute(command: WorkspaceCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        WorkspaceCommands::List => {
            let resp = client.get("/v2/team").await?;
            let teams = resp
                .get("teams")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            // Simplify for table output — extract id, name, member count
            let items: Vec<serde_json::Value> = teams
                .iter()
                .map(|ws| {
                    serde_json::json!({
                        "id": ws.get("id").and_then(|v| v.as_str()).unwrap_or("-"),
                        "name": ws.get("name").and_then(|v| v.as_str()).unwrap_or("-"),
                        "members": ws.get("members").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
                    })
                })
                .collect();
            output.print_items(&items, &["id", "name", "members"], "id");
            Ok(())
        }
        WorkspaceCommands::Seats => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client.get(&format!("/v2/team/{}/seats", ws_id)).await?;
            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                // seats response has filled_members_seats, empty_members_seats, etc.
                let filled = resp.get("seats").and_then(|s| s.get("members"))
                    .and_then(|m| m.get("filled_members_seats"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let total = resp.get("seats").and_then(|s| s.get("members"))
                    .and_then(|m| m.get("total_members_seats"))
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        // Try alternative: top-level fields
                        resp.get("filled_members_seats").and_then(|v| v.as_u64())
                    })
                    .unwrap_or(0);
                let items = vec![serde_json::json!({
                    "filled_seats": filled,
                    "total_seats": total,
                })];
                output.print_items(&items, &["filled_seats", "total_seats"], "filled_seats");
            }
            Ok(())
        }
        WorkspaceCommands::Plan => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client.get(&format!("/v2/team/{}/plan", ws_id)).await?;
            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                let plan_name = resp.get("plan_id")
                    .or_else(|| resp.get("plan_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                println!("Plan: {}", plan_name);
            }
            Ok(())
        }
    }
}
