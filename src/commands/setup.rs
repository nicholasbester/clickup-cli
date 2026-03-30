use clap::Args;
use crate::client::ClickUpClient;
use crate::config::{AuthConfig, Config, DefaultsConfig};
use crate::error::CliError;
use crate::Cli;
use std::io::{self, Write};

#[derive(Args)]
pub struct SetupArgs {
    /// API token (skip interactive prompt)
    #[arg(long)]
    pub token: Option<String>,
}

pub async fn execute(args: SetupArgs, cli: &Cli) -> Result<(), CliError> {
    let token = match args.token.or_else(|| cli.token.clone()) {
        Some(t) => t,
        None => prompt_token()?,
    };

    // Validate token by hitting /v2/user
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let user_resp = client.get("/v2/user").await?;
    let username = user_resp
        .get("user")
        .and_then(|u| u.get("username"))
        .and_then(|u| u.as_str())
        .unwrap_or("Unknown");
    eprintln!("Validating... ✓ Authenticated as {}", username);

    // Fetch workspaces
    let teams_resp = client.get("/v2/team").await?;
    let teams = teams_resp
        .get("teams")
        .and_then(|t| t.as_array())
        .cloned()
        .unwrap_or_default();

    let workspace_id = match teams.len() {
        0 => {
            return Err(CliError::ClientError {
                message: "No workspaces found for this token".into(),
                status: 0,
            });
        }
        1 => {
            let ws = &teams[0];
            let id = ws.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let name = ws.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            eprintln!("\nOnly one workspace found — setting as default.");
            eprintln!("  {} (ID: {})", name, id);
            id.to_string()
        }
        _ => {
            eprintln!("\nFetching workspaces...");
            for (i, ws) in teams.iter().enumerate() {
                let id = ws.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let name = ws.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
                eprintln!("  [{}] {} (ID: {})", i + 1, name, id);
            }
            let choice = prompt_choice(teams.len())?;
            let ws = &teams[choice - 1];
            ws.get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        }
    };

    let config = Config {
        auth: AuthConfig {
            token: token.clone(),
        },
        defaults: DefaultsConfig {
            workspace_id: Some(workspace_id),
            output: None,
        },
    };
    config.save()?;

    let path = Config::config_path()?;
    eprintln!("Config saved to {}", path.display());
    Ok(())
}

fn prompt_token() -> Result<String, CliError> {
    eprint!("API Token (get one at Settings > Apps): ");
    io::stderr().flush()?;
    let mut token = String::new();
    io::stdin().read_line(&mut token)?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(CliError::ConfigError("No token provided".into()));
    }
    Ok(token)
}

fn prompt_choice(max: usize) -> Result<usize, CliError> {
    eprint!("\nSelect workspace [1-{}]: ", max);
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice: usize = input
        .trim()
        .parse()
        .map_err(|_| CliError::ConfigError("Invalid selection".into()))?;
    if choice < 1 || choice > max {
        return Err(CliError::ConfigError(format!("Selection must be between 1 and {}", max)));
    }
    Ok(choice)
}
