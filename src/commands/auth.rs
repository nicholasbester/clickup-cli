use crate::auth_token::{AuthToken, TokenKind};
use crate::client::ClickUpClient;
use crate::config::{Config, TokenStorage};
use crate::credentials::{clear_token, CredentialStore, SystemKeychain};
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Show current user info
    Whoami,
    /// Quick token validation (exit code only)
    Check,
    /// Authorize your own ClickUp OAuth app in a browser
    Login(super::auth_oauth::LoginArgs),
    /// Remove stored credentials from the nearest project and global config
    Logout,
    /// Validate credentials and report user, kind, source and workspace (no token)
    Status,
}

pub async fn execute(command: AuthCommands, cli: &Cli) -> Result<(), CliError> {
    match command {
        AuthCommands::Login(args) => return super::auth_oauth::login(args, cli).await,
        AuthCommands::Logout => return logout(),
        _ => {}
    }
    let resolved = resolve_credentials(cli.token.as_deref(), cli.token_kind)?;
    let client = ClickUpClient::new(&resolved.token, cli.timeout)?;
    let resp = client.get("/v2/user").await?;
    match command {
        AuthCommands::Whoami => {
            let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
            if let Some(user) = resp.get("user") {
                output.print_single(user, &["id", "username", "email"], "id");
            }
        }
        AuthCommands::Status => {
            let workspace = super::workspace::resolve_workspace(cli).ok();
            let value = serde_json::json!({
                "authenticated": true,
                "username": resp.pointer("/user/username").and_then(|v| v.as_str()).unwrap_or("Unknown"),
                "kind": resolved.token.kind.as_str(),
                "source": resolved.source,
                "workspace_id": workspace,
            });
            OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet)
                .print_single(
                    &value,
                    &[
                        "authenticated",
                        "username",
                        "kind",
                        "source",
                        "workspace_id",
                    ],
                    "username",
                );
        }
        _ => {}
    }
    Ok(())
}

pub struct ResolvedCredentials {
    pub token: AuthToken,
    pub source: &'static str,
}

pub fn resolve_token(cli: &Cli) -> Result<AuthToken, CliError> {
    Ok(resolve_credentials(cli.token.as_deref(), cli.token_kind)?.token)
}

pub fn resolve_credentials(
    flag: Option<&str>,
    kind: Option<TokenKind>,
) -> Result<ResolvedCredentials, CliError> {
    resolve_with(
        flag,
        kind,
        env_token("CLICKUP_TOKEN"),
        env_token("CLICKUP_OAUTH_TOKEN"),
        Config::load,
        &SystemKeychain,
    )
}

fn env_token(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

// Explicit inputs keep precedence tests isolated from process environment and native keychains.
pub fn resolve_with(
    flag: Option<&str>,
    kind: Option<TokenKind>,
    personal: Option<String>,
    oauth: Option<String>,
    config: impl FnOnce() -> Result<Config, CliError>,
    store: &dyn CredentialStore,
) -> Result<ResolvedCredentials, CliError> {
    let (token, source) = if let Some(value) = flag {
        (
            AuthToken {
                kind: kind.unwrap_or_default(),
                token: value.into(),
            },
            "flag",
        )
    } else if let Some(value) = personal.filter(|s| !s.is_empty()) {
        (AuthToken::personal(value), "CLICKUP_TOKEN")
    } else if let Some(value) = oauth.filter(|s| !s.is_empty()) {
        (
            AuthToken {
                kind: TokenKind::Oauth,
                token: value,
            },
            "CLICKUP_OAUTH_TOKEN",
        )
    } else {
        let config = config()?;
        let source = if config.auth.storage == TokenStorage::Keychain {
            "keychain"
        } else {
            "file"
        };
        (config.auth.load_token(store)?, source)
    };
    token.validate()?;
    Ok(ResolvedCredentials { token, source })
}

fn logout() -> Result<(), CliError> {
    let mut paths = vec![Config::config_path()?];
    if std::env::var_os("CLICKUP_CONFIG").is_none() {
        if let Some(path) = std::env::current_dir()
            .ok()
            .and_then(|p| Config::find_project_config(&p))
        {
            if !paths.contains(&path) {
                paths.insert(0, path);
            }
        }
    }
    for path in paths {
        if path.exists() {
            let mut config = Config::load_from(&path)?;
            clear_token(&mut config, &path, &SystemKeychain)?;
            eprintln!("Removed credentials from {}", path.display());
        }
    }
    eprintln!("Logged out of stored credentials. Environment/flag tokens are unchanged. Revoke OAuth app access in ClickUp Settings > Apps if needed.");
    Ok(())
}
