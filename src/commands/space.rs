use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum SpaceCommands {
    /// List spaces
    List {
        /// Include archived spaces
        #[arg(long)]
        archived: bool,
    },
    /// Get space details
    Get {
        /// Space ID
        id: String,
    },
    /// Create a new space
    Create {
        /// Space name
        #[arg(long)]
        name: String,
        /// Make space private
        #[arg(long)]
        private: bool,
        /// Allow multiple assignees
        #[arg(long)]
        multiple_assignees: bool,
    },
    /// Update a space
    Update {
        /// Space ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// Color hex
        #[arg(long)]
        color: Option<String>,
    },
    /// Delete a space
    Delete {
        /// Space ID
        id: String,
    },
}

pub async fn execute(_command: SpaceCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 7")
}
