use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::config::Config;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Show current user info
    Whoami,
    /// Quick token validation (exit code only)
    Check,
}

pub async fn execute(command: AuthCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;

    match command {
        AuthCommands::Whoami => {
            let resp = client.get("/v2/user").await?;
            let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
            if let Some(user) = resp.get("user") {
                output.print_single(user, &["id", "username", "email"], "id");
            }
            Ok(())
        }
        AuthCommands::Check => {
            // Just hit the endpoint — success = exit 0, failure = error propagates
            client.get("/v2/user").await?;
            Ok(())
        }
    }
}

pub fn resolve_token(cli: &Cli) -> Result<String, CliError> {
    if let Some(token) = &cli.token {
        return Ok(token.clone());
    }
    let config = Config::load()?;
    if config.auth.token.is_empty() {
        return Err(CliError::ConfigError("Not configured".into()));
    }
    Ok(config.auth.token)
}
