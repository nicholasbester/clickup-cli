use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum FolderCommands {
    /// List folders in a space
    List {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get folder details
    Get {
        /// Folder ID
        id: String,
    },
    /// Create a folder
    Create {
        /// Space ID
        #[arg(long)]
        space: String,
        /// Folder name
        #[arg(long)]
        name: String,
    },
    /// Update a folder
    Update {
        /// Folder ID
        id: String,
        /// New name
        #[arg(long)]
        name: String,
    },
    /// Delete a folder
    Delete {
        /// Folder ID
        id: String,
    },
}

pub async fn execute(_command: FolderCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 8")
}
