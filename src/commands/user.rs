use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum UserCommands {
    /// Invite a user to the workspace
    Invite {
        /// Email address
        #[arg(long)]
        email: String,
        /// Grant admin role
        #[arg(long)]
        admin: bool,
        /// Custom role ID
        #[arg(long)]
        custom_role_id: Option<String>,
    },
    /// Get a workspace member by user ID
    Get {
        /// User ID
        id: String,
    },
    /// Update a workspace member
    Update {
        /// User ID
        id: String,
        /// New username
        #[arg(long)]
        username: Option<String>,
        /// Set admin role
        #[arg(long)]
        admin: Option<bool>,
        /// Custom role ID
        #[arg(long)]
        custom_role_id: Option<String>,
    },
    /// Remove a user from the workspace
    Remove {
        /// User ID
        id: String,
    },
}

const USER_FIELDS: &[&str] = &["id", "username", "email", "role"];

pub async fn execute(command: UserCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);
    let ws_id = resolve_workspace(cli)?;

    match command {
        UserCommands::Invite {
            email,
            admin,
            custom_role_id,
        } => {
            let mut body = serde_json::json!({
                "email": email,
                "admin": admin,
            });
            if let Some(r) = custom_role_id {
                body["custom_role_id"] = serde_json::Value::String(r);
            }
            let resp = client
                .post(&format!("/v2/team/{}/user", ws_id), &body)
                .await?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            let user = member.get("user").cloned().unwrap_or(member);
            output.print_single(&user, USER_FIELDS, "id");
            Ok(())
        }
        UserCommands::Get { id } => {
            let resp = client
                .get(&format!("/v2/team/{}/user/{}", ws_id, id))
                .await?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            let user = member.get("user").cloned().unwrap_or(member);
            output.print_single(&user, USER_FIELDS, "id");
            Ok(())
        }
        UserCommands::Update {
            id,
            username,
            admin,
            custom_role_id,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(u) = username {
                body.insert("username".into(), serde_json::Value::String(u));
            }
            if let Some(a) = admin {
                body.insert("admin".into(), serde_json::Value::Bool(a));
            }
            if let Some(r) = custom_role_id {
                body.insert("custom_role_id".into(), serde_json::Value::String(r));
            }
            let resp = client
                .put(
                    &format!("/v2/team/{}/user/{}", ws_id, id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            let member = resp.get("member").cloned().unwrap_or(resp);
            let user = member.get("user").cloned().unwrap_or(member);
            output.print_single(&user, USER_FIELDS, "id");
            Ok(())
        }
        UserCommands::Remove { id } => {
            client
                .delete(&format!("/v2/team/{}/user/{}", ws_id, id))
                .await?;
            output.print_message(&format!("User {} removed from workspace", id));
            Ok(())
        }
    }
}
