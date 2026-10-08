use crate::commands::auth::resolve_credentials;
use crate::config::Config;
use crate::error::CliError;
use crate::Cli;

pub async fn execute(cli: &Cli) -> Result<(), CliError> {
    println!("clickup-cli v{}\n", env!("CARGO_PKG_VERSION"));
    if let Ok(path) = Config::active_path() {
        println!("Config:    {}", path.display());
    }
    match resolve_credentials(cli.token.as_deref(), cli.token_kind) {
        Ok(auth) => {
            println!("Token:     (configured; not validated)");
            println!("Auth:      {}", auth.token.kind.as_str());
            println!("Source:    {}", auth.source);
        }
        Err(err) => println!("Auth:      {}", err),
    }
    match super::workspace::resolve_workspace(cli) {
        Ok(ws) => println!("Workspace: {}", ws),
        Err(_) => println!("Workspace: (not configured)"),
    }
    println!("Use 'clickup-cli auth status' to validate the effective credentials.");
    Ok(())
}
