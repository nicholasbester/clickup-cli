use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum WorkspaceCommands {
    /// List workspaces
    List,
    /// Show seat usage
    Seats,
    /// Show current plan
    Plan,
}

pub async fn execute(_command: WorkspaceCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 6")
}
