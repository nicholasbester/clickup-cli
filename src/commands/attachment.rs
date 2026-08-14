use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::git;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AttachmentCommands {
    /// Upload a file attachment to a task
    Upload {
        /// Path to the file to upload
        file: std::path::PathBuf,
        /// Task ID (auto-detected from git branch if omitted)
        #[arg(long)]
        task: Option<String>,
    },
    /// List attachments on a task (extracted from the Get Task response)
    List {
        /// Task ID (auto-detected from git branch if omitted)
        #[arg(long)]
        task: Option<String>,
    },
}

const ATTACHMENT_FIELDS: &[&str] = &["id", "title", "url", "date"];

pub async fn execute(command: AttachmentCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        AttachmentCommands::Upload { task, file } => {
            let task = git::require_task(cli, task.as_deref(), true)?;
            let q = crate::commands::workspace::custom_task_query(cli, &task, '?')?;
            let resp = client
                .upload_file(&format!("/v2/task/{}/attachment{}", task.id, q), &file)
                .await?;
            output.print_single(&resp, ATTACHMENT_FIELDS, "id");
            Ok(())
        }
        AttachmentCommands::List { task } => {
            // ClickUp has no dedicated list-attachments endpoint. The `attachments`
            // array is returned inline by GET /v2/task/{id}, per the API docs.
            let task = git::require_task(cli, task.as_deref(), true)?;
            let q = crate::commands::workspace::custom_task_query(cli, &task, '?')?;
            let resp = client.get(&format!("/v2/task/{}{}", task.id, q)).await?;
            let attachments = resp
                .get("attachments")
                .and_then(|a| a.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&attachments, ATTACHMENT_FIELDS, "id");
            Ok(())
        }
    }
}
