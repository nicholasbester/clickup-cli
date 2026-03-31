use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum GroupCommands {
    /// List groups in the workspace
    List,
    /// Create a group
    Create {
        /// Group name
        #[arg(long)]
        name: String,
        /// Member user IDs (repeat for multiple)
        #[arg(long = "member")]
        members: Vec<String>,
    },
    /// Update a group
    Update {
        /// Group ID
        id: String,
        /// New group name
        #[arg(long)]
        name: Option<String>,
        /// User IDs to add (repeat for multiple)
        #[arg(long = "add-member")]
        add_members: Vec<String>,
        /// User IDs to remove (repeat for multiple)
        #[arg(long = "rem-member")]
        rem_members: Vec<String>,
    },
    /// Delete a group
    Delete {
        /// Group ID
        id: String,
    },
}

const GROUP_FIELDS: &[&str] = &["id", "name", "members"];

pub async fn execute(command: GroupCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        GroupCommands::List => {
            let resp = client.get("/v2/group").await?;
            let groups = resp
                .get("groups")
                .and_then(|g| g.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&groups, GROUP_FIELDS, "id");
            Ok(())
        }
        GroupCommands::Create { name, members } => {
            let team_id = resolve_workspace(cli)?;
            let body = serde_json::json!({
                "name": name,
                "member_ids": members,
            });
            let resp = client
                .post(&format!("/v2/team/{}/group", team_id), &body)
                .await?;
            let group = resp.get("group").cloned().unwrap_or(resp);
            output.print_single(&group, GROUP_FIELDS, "id");
            Ok(())
        }
        GroupCommands::Update {
            id,
            name,
            add_members,
            rem_members,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if !add_members.is_empty() || !rem_members.is_empty() {
                let add: Vec<serde_json::Value> = add_members
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect();
                let rem: Vec<serde_json::Value> = rem_members
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect();
                body.insert(
                    "members".into(),
                    serde_json::json!({ "add": add, "rem": rem }),
                );
            }
            let resp = client
                .put(
                    &format!("/v2/group/{}", id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            let group = resp.get("group").cloned().unwrap_or(resp);
            output.print_single(&group, GROUP_FIELDS, "id");
            Ok(())
        }
        GroupCommands::Delete { id } => {
            client.delete(&format!("/v2/group/{}", id)).await?;
            output.print_message(&format!("Group {} deleted", id));
            Ok(())
        }
    }
}
