use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List task templates for the workspace
    List {
        /// Page number
        #[arg(long, default_value = "0")]
        page: u32,
    },
    /// Apply a task template to a list
    #[command(name = "apply-task")]
    ApplyTask {
        /// Template ID
        template_id: String,
        /// List ID to create the task in
        #[arg(long)]
        list: String,
        /// Task name
        #[arg(long)]
        name: String,
    },
    /// Apply a list template to a folder or space
    #[command(name = "apply-list")]
    ApplyList {
        /// Template ID
        template_id: String,
        /// Folder ID (mutually exclusive with --space)
        #[arg(long, conflicts_with = "space")]
        folder: Option<String>,
        /// Space ID (mutually exclusive with --folder)
        #[arg(long)]
        space: Option<String>,
        /// List name
        #[arg(long)]
        name: String,
    },
    /// Apply a folder template to a space
    #[command(name = "apply-folder")]
    ApplyFolder {
        /// Template ID
        template_id: String,
        /// Space ID
        #[arg(long)]
        space: String,
        /// Folder name
        #[arg(long)]
        name: String,
    },
}

const TEMPLATE_FIELDS: &[&str] = &["id", "name"];

pub async fn execute(command: TemplateCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        TemplateCommands::List { page } => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/taskTemplate?page={}", ws_id, page))
                .await?;
            let mut templates = resp
                .get("templates")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = cli.limit {
                templates.truncate(limit);
            }
            output.print_items(&templates, TEMPLATE_FIELDS, "id");
            Ok(())
        }
        TemplateCommands::ApplyTask {
            template_id,
            list,
            name,
        } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .post(
                    &format!("/v2/list/{}/taskTemplate/{}", list, template_id),
                    &body,
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            Ok(())
        }
        TemplateCommands::ApplyList {
            template_id,
            folder,
            space,
            name,
        } => {
            let body = serde_json::json!({ "name": name });
            let path = if let Some(f) = folder {
                format!("/v2/folder/{}/list_template/{}", f, template_id)
            } else if let Some(s) = space {
                format!("/v2/space/{}/list_template/{}", s, template_id)
            } else {
                return Err(CliError::ClientError {
                    message: "Specify --folder or --space".into(),
                    status: 0,
                });
            };
            let resp = client.post(&path, &body).await?;
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            Ok(())
        }
        TemplateCommands::ApplyFolder {
            template_id,
            space,
            name,
        } => {
            let body = serde_json::json!({ "name": name });
            let resp = client
                .post(
                    &format!("/v2/space/{}/folder_template/{}", space, template_id),
                    &body,
                )
                .await?;
            println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            Ok(())
        }
    }
}
