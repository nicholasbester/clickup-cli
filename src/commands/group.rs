use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

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
            // ClickUp's GET /v2/group requires team_id as a query param.
            let team_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/group?team_id={}", team_id))
                .await?;
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
            // ClickUp's spec: body field is `members` (not `member_ids`) and the
            // array contains integer user IDs (not strings). Parse and bail on
            // anything that isn't a positive integer.
            let member_ids: Result<Vec<i64>, _> = members
                .iter()
                .map(|m| m.parse::<i64>().map_err(|_| m.clone()))
                .collect();
            let member_ids = member_ids.map_err(|bad| CliError::ClientError {
                message: format!("--member must be a numeric user id, got '{}'", bad),
                status: 0,
            })?;
            let body = serde_json::json!({
                "name": name,
                "members": member_ids,
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
                // ClickUp's spec: member IDs in add/rem arrays are integers.
                let parse = |ids: Vec<String>| -> Result<Vec<serde_json::Value>, CliError> {
                    ids.into_iter()
                        .map(|s| {
                            s.parse::<i64>().map(|n| serde_json::json!(n)).map_err(|_| {
                                CliError::ClientError {
                                    message: format!(
                                        "member id must be a numeric user id, got '{}'",
                                        s
                                    ),
                                    status: 0,
                                }
                            })
                        })
                        .collect()
                };
                let add = parse(add_members)?;
                let rem = parse(rem_members)?;
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
