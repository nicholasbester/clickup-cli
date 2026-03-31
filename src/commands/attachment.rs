use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum AttachmentCommands {
    /// Upload a file attachment to a task
    Upload {
        /// Task ID
        #[arg(long)]
        task: String,
        /// Path to the file to upload
        file: std::path::PathBuf,
    },
}

const ATTACHMENT_FIELDS: &[&str] = &["id", "title", "url", "date"];

pub async fn execute(command: AttachmentCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        AttachmentCommands::Upload { task, file } => {
            let resp = client
                .upload_file(&format!("/v2/task/{}/attachment", task), &file)
                .await?;
            output.print_single(&resp, ATTACHMENT_FIELDS, "id");
            Ok(())
        }
    }
}
