pub mod client;
pub mod commands;
pub mod config;
pub mod error;
pub mod models;
pub mod output;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "clickup", version, about = "CLI for the ClickUp API")]
pub struct Cli {
    /// API token (overrides config file)
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Workspace ID (overrides config default)
    #[arg(long, global = true)]
    pub workspace: Option<String>,

    /// Output format: table, json, json-compact, csv
    #[arg(long, global = true, default_value = "table")]
    pub output: String,

    /// Comma-separated list of fields to display
    #[arg(long, global = true)]
    pub fields: Option<String>,

    /// Omit table header row
    #[arg(long, global = true)]
    pub no_header: bool,

    /// Fetch all pages
    #[arg(long, global = true)]
    pub all: bool,

    /// Cap total results
    #[arg(long, global = true)]
    pub limit: Option<usize>,

    /// Manual page selection
    #[arg(long, global = true)]
    pub page: Option<u32>,

    /// Only print IDs, one per line
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// HTTP timeout in seconds
    #[arg(long, global = true, default_value = "30")]
    pub timeout: u64,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Configure API token and default workspace
    Setup(commands::setup::SetupArgs),
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        command: commands::auth::AuthCommands,
    },
    /// Workspace commands
    Workspace {
        #[command(subcommand)]
        command: commands::workspace::WorkspaceCommands,
    },
    /// Space commands
    Space {
        #[command(subcommand)]
        command: commands::space::SpaceCommands,
    },
    /// Folder commands
    Folder {
        #[command(subcommand)]
        command: commands::folder::FolderCommands,
    },
    /// List commands
    List {
        #[command(subcommand)]
        command: commands::list::ListCommands,
    },
    /// Task commands
    Task {
        #[command(subcommand)]
        command: commands::task::TaskCommands,
    },
    /// Checklist commands
    Checklist {
        #[command(subcommand)]
        command: commands::checklist::ChecklistCommands,
    },
    /// Comment commands
    Comment {
        #[command(subcommand)]
        command: commands::comment::CommentCommands,
    },
    /// Tag commands
    Tag {
        #[command(subcommand)]
        command: commands::tag::TagCommands,
    },
    /// Custom field commands
    Field {
        #[command(subcommand)]
        command: commands::field::FieldCommands,
    },
    /// Custom task type commands
    #[command(name = "task-type")]
    TaskType {
        #[command(subcommand)]
        command: commands::task_type::TaskTypeCommands,
    },
    /// Attachment commands
    Attachment {
        #[command(subcommand)]
        command: commands::attachment::AttachmentCommands,
    },
    /// Time tracking commands
    Time {
        #[command(subcommand)]
        command: commands::time::TimeCommands,
    },
    /// Goal commands
    Goal {
        #[command(subcommand)]
        command: commands::goal::GoalCommands,
    },
    /// View commands
    View {
        #[command(subcommand)]
        command: commands::view::ViewCommands,
    },
    /// Member commands
    Member {
        #[command(subcommand)]
        command: commands::member::MemberCommands,
    },
    /// User commands
    User {
        #[command(subcommand)]
        command: commands::user::UserCommands,
    },
    /// Chat commands (v3)
    Chat {
        #[command(subcommand)]
        command: commands::chat::ChatCommands,
    },
    /// Doc commands (v3)
    Doc {
        #[command(subcommand)]
        command: commands::doc::DocCommands,
    },
    /// Webhook commands
    Webhook {
        #[command(subcommand)]
        command: commands::webhook::WebhookCommands,
    },
    /// Template commands
    Template {
        #[command(subcommand)]
        command: commands::template::TemplateCommands,
    },
}
