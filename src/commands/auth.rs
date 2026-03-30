use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Show current user info
    Whoami,
    /// Quick token validation (exit code only)
    Check,
}

pub async fn execute(_command: AuthCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 5")
}
