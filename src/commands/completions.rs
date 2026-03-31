use clap::CommandFactory;
use clap_complete::{generate, Shell};
use crate::error::CliError;
use crate::Cli;

pub fn execute(shell: Shell) -> Result<(), CliError> {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "clickup", &mut std::io::stdout());
    Ok(())
}
