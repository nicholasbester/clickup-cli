use crate::client::ClickUpClient;
use crate::config::{AuthConfig, Config, DefaultsConfig};
use crate::error::CliError;
use crate::Cli;
use clap::Args;
use std::io::{self, Write};

#[derive(Args)]
pub struct SetupArgs {
    /// API token (skip interactive prompt)
    #[arg(long)]
    pub token: Option<String>,
}

pub async fn execute(args: SetupArgs, cli: &Cli) -> Result<(), CliError> {
    if cli.token_kind == Some(crate::auth_token::TokenKind::Oauth) {
        return Err(CliError::ConfigError("setup configures personal tokens; use auth login for OAuth, or CLICKUP_OAUTH_TOKEN for an existing OAuth token".into()));
    }
    let token = match args.token.or_else(|| cli.token.clone()) {
        Some(t) => t,
        None => prompt_token()?,
    };

    // Validate token by hitting /v2/user
    let client = ClickUpClient::new(&crate::auth_token::AuthToken::personal(&token), cli.timeout)?;
    let workspace_id = validate_and_select_workspace(&client, None).await?;

    let mut config = Config {
        auth: AuthConfig {
            token: token.clone(),
            ..Default::default()
        },
        defaults: DefaultsConfig {
            workspace_id: Some(workspace_id),
            output: None,
        },
        git: Default::default(),
    };
    let path = Config::config_path()?;
    // Retain the previous storage marker long enough to remove a replaced keychain token.
    if path.exists() {
        config.auth = Config::load_from(&path)?.auth;
    }
    crate::credentials::save_token(
        &mut config,
        &path,
        crate::auth_token::AuthToken::personal(token),
        crate::config::TokenStorage::File,
        &crate::credentials::SystemKeychain,
    )?;

    eprintln!("Config saved to {}", path.display());
    Ok(())
}

pub(crate) async fn validate_and_select_workspace(
    client: &ClickUpClient,
    preferred: Option<&str>,
) -> Result<String, CliError> {
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

    if let Some(id) = preferred {
        if teams
            .iter()
            .any(|t| t.get("id").and_then(|v| v.as_str()) == Some(id))
        {
            return Ok(id.to_string());
        }
        return Err(CliError::ConfigError(
            "Requested workspace is not authorized for this token".into(),
        ));
    }
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

    Ok(workspace_id)
}

fn prompt_token() -> Result<String, CliError> {
    eprint!("API Token (Settings > Apps; or use clickup-cli auth login for OAuth): ");
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
        return Err(CliError::ConfigError(format!(
            "Selection must be between 1 and {}",
            max
        )));
    }
    Ok(choice)
}
