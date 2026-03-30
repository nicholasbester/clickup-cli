use clap::Args;
use crate::Cli;
use crate::error::CliError;

#[derive(Args)]
pub struct SetupArgs {
    /// API token (skip interactive prompt)
    #[arg(long)]
    pub token: Option<String>,
}

pub async fn execute(_args: SetupArgs, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 4")
}
