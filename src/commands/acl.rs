use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AclCommands {
    /// Update ACL for an object (Enterprise only, v3)
    Update {
        /// Object type (e.g. task, list, folder, space)
        object_type: String,
        /// Object ID
        object_id: String,
        /// Mark the object private (true) or remove the private flag (false). Omit to leave unchanged.
        #[arg(long)]
        private: Option<bool>,
        /// Grant a user permission. Format: USER_ID[:LEVEL] where LEVEL is read|comment|edit|create (default: read). Repeat for multiple users.
        #[arg(long = "grant-user")]
        grant_user: Vec<String>,
        /// Grant a group permission. Same format as --grant-user but the id refers to a user group.
        #[arg(long = "grant-group")]
        grant_group: Vec<String>,
        /// Revoke a user (sends permission_level=0). Repeat for multiple users.
        #[arg(long = "revoke-user")]
        revoke_user: Vec<String>,
        /// Revoke a group (sends permission_level=0). Repeat for multiple groups.
        #[arg(long = "revoke-group")]
        revoke_group: Vec<String>,
        /// Raw JSON body (overrides all other flags). Use this for advanced shapes the flags don't cover.
        #[arg(long)]
        body: Option<String>,
    },
}

fn permission_to_level(name: &str) -> Result<u8, CliError> {
    // ClickUp's documented permission_level integers on ACL entries.
    // The API enum is 1, 3, 4, 5 (0 used here for revocation).
    match name.to_lowercase().as_str() {
        "read" | "1" => Ok(1),
        "comment" | "3" => Ok(3),
        "edit" | "4" => Ok(4),
        "create" | "5" => Ok(5),
        other => Err(CliError::ClientError {
            message: format!(
                "Unknown permission '{}'. Valid: read (1), comment (3), edit (4), create (5).",
                other
            ),
            status: 0,
        }),
    }
}

fn parse_grant(raw: &str, kind: &'static str) -> Result<serde_json::Value, CliError> {
    // Format: ID[:LEVEL]. Default level is read (1).
    let (id, level) = match raw.split_once(':') {
        Some((id, level)) => (id.trim().to_string(), permission_to_level(level.trim())?),
        None => (raw.trim().to_string(), 1u8),
    };
    Ok(serde_json::json!({
        "kind": kind,
        "id": id,
        "permission_level": level,
    }))
}

fn revoke_entry(id: &str, kind: &'static str) -> serde_json::Value {
    // permission_level=0 to indicate removal. ClickUp's spec treats
    // permission_level as optional and 0 is conventionally used to revoke.
    // If a workspace requires a different shape, use --body for raw JSON.
    serde_json::json!({
        "kind": kind,
        "id": id,
        "permission_level": 0,
    })
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
            grant_group,
            revoke_user,
            revoke_group,
            body,
        } => {
            let team_id = resolve_workspace(cli)?;

            // ClickUp's PATCH /v3/workspaces/{ws}/{type}/{id}/acls body shape
            // per the v3 OpenAPI spec:
            //   { "private"?: bool, "entries"?: [ { kind, id, permission_level? } ] }
            // The previous implementation invented `{access_type, grant, revoke}`,
            // which the endpoint does not recognise.
            let request_body = if let Some(raw) = body {
                serde_json::from_str(&raw).map_err(|e| CliError::ClientError {
                    message: format!("Invalid JSON body: {}", e),
                    status: 0,
                })?
            } else {
                let mut b = serde_json::Map::new();
                if let Some(p) = private {
                    b.insert("private".into(), serde_json::Value::Bool(p));
                }
                let mut entries: Vec<serde_json::Value> = Vec::new();
                for raw in grant_user {
                    entries.push(parse_grant(&raw, "user")?);
                }
                for raw in grant_group {
                    entries.push(parse_grant(&raw, "group")?);
                }
                for id in revoke_user {
                    entries.push(revoke_entry(&id, "user"));
                }
                for id in revoke_group {
                    entries.push(revoke_entry(&id, "group"));
                }
                if !entries.is_empty() {
                    b.insert("entries".into(), serde_json::Value::Array(entries));
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
                output.print_message(&format!("ACL updated for {} {}", object_type, object_id));
            }
            Ok(())
        }
    }
}
