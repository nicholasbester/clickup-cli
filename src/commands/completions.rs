use crate::error::CliError;
use crate::Cli;
use clap::CommandFactory;
use clap_complete::{generate, Shell};

pub fn execute(shell: Shell) -> Result<(), CliError> {
    let mut cmd = Cli::command();
    // Generate completions under the canonical binary name. Users who installed
    // the `clkup` short alias can run `clkup completions <shell>` themselves; the
    // generated script will still work because clap's command name drives the
    // completion file's function names but the script invokes whichever binary
    // it's saved as.
    generate(shell, &mut cmd, "clickup-cli", &mut std::io::stdout());
    Ok(())
}
