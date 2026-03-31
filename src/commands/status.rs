use crate::config::Config;
use crate::error::CliError;
use crate::Cli;

pub async fn execute(cli: &Cli) -> Result<(), CliError> {
    println!("clickup-cli v{}", env!("CARGO_PKG_VERSION"));
    println!();

    // Config
    match Config::config_path() {
        Ok(path) => println!("Config:    {}", path.display()),
        Err(_) => println!("Config:    (unknown path)"),
    }

    // Auth
    match Config::load() {
        Ok(config) => {
            let token = &config.auth.token;
            if token.is_empty() {
                println!("Token:     (not set)");
            } else {
                let masked = format!("{}...{}", &token[..6.min(token.len())], &token[token.len().saturating_sub(4)..]);
                println!("Token:     {}", masked);
            }
            match &config.defaults.workspace_id {
                Some(ws) => println!("Workspace: {}", ws),
                None => println!("Workspace: (not set)"),
            }
        }
        Err(_) => {
            println!("Token:     (not configured)");
            println!("Workspace: (not configured)");
            println!();
            println!("Run 'clickup setup' to configure.");
            return Ok(());
        }
    }

    // Env overrides
    if std::env::var("CLICKUP_TOKEN").is_ok() {
        println!("           (CLICKUP_TOKEN env var set — overrides config)");
    }
    if std::env::var("CLICKUP_WORKSPACE").is_ok() {
        println!("           (CLICKUP_WORKSPACE env var set — overrides config)");
    }
    if cli.token.is_some() {
        println!("           (--token flag set — overrides all)");
    }

    Ok(())
}
