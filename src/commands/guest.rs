use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum GuestCommands {
    /// Invite a guest to the workspace (Enterprise only)
    Invite {
        /// Guest email address
        #[arg(long)]
        email: String,
        /// Allow guest to edit tags
        #[arg(long)]
        can_edit_tags: bool,
        /// Allow guest to see time spent
        #[arg(long)]
        can_see_time_spent: bool,
        /// Allow guest to create views
        #[arg(long)]
        can_create_views: bool,
        /// Custom role ID to assign
        #[arg(long)]
        custom_role_id: Option<u64>,
    },
    /// Get a guest by ID
    Get {
        /// Guest ID
        id: String,
    },
    /// Update guest permissions
    Update {
        /// Guest ID
        id: String,
        /// Allow guest to edit tags
        #[arg(long)]
        can_edit_tags: Option<bool>,
        /// Allow guest to see time spent
        #[arg(long)]
        can_see_time_spent: Option<bool>,
        /// Allow guest to create views
        #[arg(long)]
        can_create_views: Option<bool>,
        /// Custom role ID to assign
        #[arg(long)]
        custom_role_id: Option<u64>,
    },
    /// Remove a guest from the workspace
    Remove {
        /// Guest ID
        id: String,
    },
    /// Share a task with a guest
    ShareTask {
        /// Task ID
        task_id: String,
        /// Guest ID
        guest_id: String,
        /// Permission level: read, comment, edit, create
        #[arg(long)]
        permission: String,
    },
    /// Unshare a task from a guest
    UnshareTask {
        /// Task ID
        task_id: String,
        /// Guest ID
        guest_id: String,
    },
    /// Share a list with a guest
    ShareList {
        /// List ID
        list_id: String,
        /// Guest ID
        guest_id: String,
        /// Permission level: read, comment, edit, create
        #[arg(long)]
        permission: String,
    },
    /// Unshare a list from a guest
    UnshareList {
        /// List ID
        list_id: String,
        /// Guest ID
        guest_id: String,
    },
    /// Share a folder with a guest
    ShareFolder {
        /// Folder ID
        folder_id: String,
        /// Guest ID
        guest_id: String,
        /// Permission level: read, comment, edit, create
        #[arg(long)]
        permission: String,
    },
    /// Unshare a folder from a guest
    UnshareFolder {
        /// Folder ID
        folder_id: String,
        /// Guest ID
        guest_id: String,
    },
}

const GUEST_FIELDS: &[&str] = &["id", "username", "email", "role"];

pub async fn execute(command: GuestCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        GuestCommands::Invite {
            email,
            can_edit_tags,
            can_see_time_spent,
            can_create_views,
            custom_role_id,
        } => {
            let team_id = resolve_workspace(cli)?;
            let mut body = serde_json::json!({
                "email": email,
                "can_edit_tags": can_edit_tags,
                "can_see_time_spent": can_see_time_spent,
                "can_create_views": can_create_views,
            });
            if let Some(role_id) = custom_role_id {
                body["custom_role_id"] = serde_json::Value::Number(role_id.into());
            }
            let resp = client
                .post(&format!("/v2/team/{}/guest", team_id), &body)
                .await?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            output.print_single(&guest, GUEST_FIELDS, "id");
            Ok(())
        }
        GuestCommands::Get { id } => {
            let team_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/guest/{}", team_id, id))
                .await?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            output.print_single(&guest, GUEST_FIELDS, "id");
            Ok(())
        }
        GuestCommands::Update {
            id,
            can_edit_tags,
            can_see_time_spent,
            can_create_views,
            custom_role_id,
        } => {
            let team_id = resolve_workspace(cli)?;
            let mut body = serde_json::Map::new();
            if let Some(v) = can_edit_tags {
                body.insert("can_edit_tags".into(), serde_json::Value::Bool(v));
            }
            if let Some(v) = can_see_time_spent {
                body.insert("can_see_time_spent".into(), serde_json::Value::Bool(v));
            }
            if let Some(v) = can_create_views {
                body.insert("can_create_views".into(), serde_json::Value::Bool(v));
            }
            if let Some(v) = custom_role_id {
                body.insert("custom_role_id".into(), serde_json::Value::Number(v.into()));
            }
            let resp = client
                .put(
                    &format!("/v2/team/{}/guest/{}", team_id, id),
                    &serde_json::Value::Object(body),
                )
                .await?;
            let guest = resp.get("guest").cloned().unwrap_or(resp);
            output.print_single(&guest, GUEST_FIELDS, "id");
            Ok(())
        }
        GuestCommands::Remove { id } => {
            let team_id = resolve_workspace(cli)?;
            client
                .delete(&format!("/v2/team/{}/guest/{}", team_id, id))
                .await?;
            output.print_message(&format!("Guest {} removed from workspace", id));
            Ok(())
        }
        GuestCommands::ShareTask {
            task_id,
            guest_id,
            permission,
        } => {
            let body = serde_json::json!({ "permission_level": permission });
            let resp = client
                .post(&format!("/v2/task/{}/guest/{}", task_id, guest_id), &body)
                .await?;
            output.print_single(&resp, GUEST_FIELDS, "id");
            Ok(())
        }
        GuestCommands::UnshareTask { task_id, guest_id } => {
            client
                .delete(&format!("/v2/task/{}/guest/{}", task_id, guest_id))
                .await?;
            output.print_message(&format!(
                "Guest {} unshared from task {}",
                guest_id, task_id
            ));
            Ok(())
        }
        GuestCommands::ShareList {
            list_id,
            guest_id,
            permission,
        } => {
            let body = serde_json::json!({ "permission_level": permission });
            let resp = client
                .post(&format!("/v2/list/{}/guest/{}", list_id, guest_id), &body)
                .await?;
            output.print_single(&resp, GUEST_FIELDS, "id");
            Ok(())
        }
        GuestCommands::UnshareList { list_id, guest_id } => {
            client
                .delete(&format!("/v2/list/{}/guest/{}", list_id, guest_id))
                .await?;
            output.print_message(&format!(
                "Guest {} unshared from list {}",
                guest_id, list_id
            ));
            Ok(())
        }
        GuestCommands::ShareFolder {
            folder_id,
            guest_id,
            permission,
        } => {
            let body = serde_json::json!({ "permission_level": permission });
            let resp = client
                .post(
                    &format!("/v2/folder/{}/guest/{}", folder_id, guest_id),
                    &body,
                )
                .await?;
            output.print_single(&resp, GUEST_FIELDS, "id");
            Ok(())
        }
        GuestCommands::UnshareFolder {
            folder_id,
            guest_id,
        } => {
            client
                .delete(&format!("/v2/folder/{}/guest/{}", folder_id, guest_id))
                .await?;
            output.print_message(&format!(
                "Guest {} unshared from folder {}",
                guest_id, folder_id
            ));
            Ok(())
        }
    }
}
