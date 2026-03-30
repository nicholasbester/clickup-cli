use clap::Subcommand;
use crate::Cli;
use crate::error::CliError;

#[derive(Subcommand)]
pub enum TaskCommands {
    /// List tasks in a list
    List {
        /// List ID
        #[arg(long)]
        list: String,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Include closed tasks
        #[arg(long)]
        include_closed: bool,
        /// Order by field
        #[arg(long)]
        order_by: Option<String>,
        /// Reverse sort order
        #[arg(long)]
        reverse: bool,
    },
    /// Search tasks across workspace
    Search {
        /// Filter by space
        #[arg(long)]
        space: Option<String>,
        /// Filter by folder
        #[arg(long)]
        folder: Option<String>,
        /// Filter by list
        #[arg(long)]
        list: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<Vec<String>>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Filter by tag
        #[arg(long)]
        tag: Option<Vec<String>>,
    },
    /// Get task details
    Get {
        /// Task ID
        id: String,
        /// Include subtasks
        #[arg(long)]
        subtasks: bool,
        /// Treat ID as custom task ID
        #[arg(long)]
        custom_task_id: bool,
    },
    /// Create a task
    Create {
        /// List ID
        #[arg(long)]
        list: String,
        /// Task name
        #[arg(long)]
        name: String,
        /// Description
        #[arg(long)]
        description: Option<String>,
        /// Status
        #[arg(long)]
        status: Option<String>,
        /// Priority (1=urgent, 2=high, 3=normal, 4=low)
        #[arg(long)]
        priority: Option<u8>,
        /// Assignee user ID
        #[arg(long)]
        assignee: Option<Vec<String>>,
        /// Tag name
        #[arg(long)]
        tag: Option<Vec<String>>,
        /// Due date (YYYY-MM-DD)
        #[arg(long)]
        due_date: Option<String>,
        /// Parent task ID (creates subtask)
        #[arg(long)]
        parent: Option<String>,
    },
    /// Update a task
    Update {
        /// Task ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New status
        #[arg(long)]
        status: Option<String>,
        /// New priority (1-4)
        #[arg(long)]
        priority: Option<u8>,
        /// Add assignee
        #[arg(long)]
        add_assignee: Option<Vec<String>>,
        /// Remove assignee
        #[arg(long)]
        rem_assignee: Option<Vec<String>>,
        /// New description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a task
    Delete {
        /// Task ID
        id: String,
    },
    /// Get time in status for a task
    TimeInStatus {
        /// Task ID(s) — multiple IDs triggers bulk mode
        ids: Vec<String>,
    },
}

pub async fn execute(_command: TaskCommands, _cli: &Cli) -> Result<(), CliError> {
    todo!("Implemented in Task 10")
}
