use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::error::CliError;
use crate::git;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum MemberCommands {
    /// List members of a task or list (use --task or --list)
    List {
        /// Task ID
        #[arg(long, conflicts_with = "list")]
        task: Option<String>,
        /// List ID
        #[arg(long, conflicts_with = "task")]
        list: Option<String>,
    },
}

const MEMBER_FIELDS: &[&str] = &["id", "username", "email", "role"];

pub async fn execute(command: MemberCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        MemberCommands::List { task, list } => {
            let url = if let Some(id) = list {
                format!("/v2/list/{}/member", id)
            } else if let Some(resolved) = git::resolve_task(cli, task.as_deref(), true)? {
                let q = crate::commands::workspace::custom_task_query(cli, &resolved, '?')?;
                format!("/v2/task/{}/member{}", resolved.id, q)
            } else {
                return Err(CliError::ClientError {
                    message: "Specify either --task ID or --list ID".into(),
                    status: 0,
                });
            };
            let resp = client.get(&url).await?;
            let members = resp
                .get("members")
                .and_then(|m| m.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&members, MEMBER_FIELDS, "id");
            Ok(())
        }
    }
}
