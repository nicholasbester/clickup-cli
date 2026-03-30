use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum ListCommands {
    /// List lists in a folder or space
    List {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless lists)
        #[arg(long)]
        space: Option<String>,
        /// Include archived
        #[arg(long)]
        archived: bool,
    },
    /// Get list details
    Get {
        /// List ID
        id: String,
    },
    /// Create a list
    Create {
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID (folderless)
        #[arg(long)]
        space: Option<String>,
        /// List name
        #[arg(long)]
        name: String,
        /// List content/description
        #[arg(long)]
        content: Option<String>,
        /// Priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
    },
    /// Update a list
    Update {
        /// List ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New content
        #[arg(long)]
        content: Option<String>,
    },
    /// Delete a list
    Delete {
        /// List ID
        id: String,
    },
    /// Add a task to this list
    AddTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
    /// Remove a task from this list
    RemoveTask {
        /// List ID
        list_id: String,
        /// Task ID
        task_id: String,
    },
}

pub async fn execute(_command: ListCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 9")
}
