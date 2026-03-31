use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum AclCommands {
    /// Update ACL for an object (Enterprise only, v3)
    Update {
        /// Object type (e.g. task, list, folder, space)
        object_type: String,
        /// Object ID
        object_id: String,
        /// Make the object private
        #[arg(long)]
        private: bool,
        /// Grant access to a user ID (use with --permission)
        #[arg(long)]
        grant_user: Option<String>,
        /// Permission level for --grant-user (e.g. read, comment, edit, create)
        #[arg(long)]
        permission: Option<String>,
        /// Revoke access from a user ID
        #[arg(long)]
        revoke_user: Option<String>,
        /// Raw JSON body (overrides all other flags)
        #[arg(long)]
        body: Option<String>,
    },
}

pub async fn execute(command: AclCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        AclCommands::Update {
            object_type,
            object_id,
            private,
            grant_user,
            permission,
            revoke_user,
            body,
        } => {
            let team_id = resolve_workspace(cli)?;
            let request_body = if let Some(raw) = body {
                serde_json::from_str(&raw).map_err(|e| CliError::ClientError {
                    message: format!("Invalid JSON body: {}", e),
                    status: 0,
                })?
            } else {
                let mut b = serde_json::Map::new();
                if private {
                    b.insert("access_type".into(), serde_json::Value::String("private".into()));
                }
                if let Some(uid) = grant_user {
                    let level = permission.unwrap_or_else(|| "read".into());
                    b.insert(
                        "grant".into(),
                        serde_json::json!([{ "user_id": uid, "permission_level": level }]),
                    );
                }
                if let Some(uid) = revoke_user {
                    b.insert(
                        "revoke".into(),
                        serde_json::json!([{ "user_id": uid }]),
                    );
                }
                serde_json::Value::Object(b)
            };

            let resp = client
                .patch(
                    &format!(
                        "/v3/workspaces/{}/{}/{}/acls",
                        team_id, object_type, object_id
                    ),
                    &request_body,
                )
                .await?;

            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
            } else {
                output.print_message(&format!(
                    "ACL updated for {} {}",
                    object_type, object_id
                ));
            }
            Ok(())
        }
    }
}
