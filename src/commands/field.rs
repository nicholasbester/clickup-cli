use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::git;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum FieldCommands {
    /// List custom fields
    List {
        /// List ID
        #[arg(long)]
        list: Option<String>,
        /// Folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Space ID
        #[arg(long)]
        space: Option<String>,
        /// Workspace-level fields
        #[arg(long = "workspace-level")]
        workspace_level: bool,
    },
    /// Set a custom field value on a task
    Set {
        /// Field ID
        field_id: String,
        /// Field value (string, number, or JSON; use the option ID for drop_down fields)
        #[arg(long)]
        value: String,
        /// Task ID (auto-detected from git branch if omitted)
        task_id: Option<String>,
    },
    /// Unset (clear) a custom field value on a task
    Unset {
        /// Field ID
        field_id: String,
        /// Task ID (auto-detected from git branch if omitted)
        task_id: Option<String>,
    },
}

const FIELD_FIELDS: &[&str] = &["id", "name", "type", "required"];

/// ClickUp custom-field IDs are UUIDs; task IDs never are (short
/// alphanumeric or `PREFIX-42` custom IDs). When the two positionals arrive
/// in the natural API order (task first) instead of the CLI's
/// `<FIELD_ID> [TASK_ID]`, reinterpret them and leave a breadcrumb on
/// stderr — otherwise the reversed URL draws a misleading
/// 401 "Team not authorized" from ClickUp (issue #96).
fn maybe_unswap(field_id: String, task_id: Option<String>) -> (String, Option<String>) {
    match task_id {
        Some(t) if is_uuid(&t) && !is_uuid(&field_id) => {
            eprintln!(
                "note: arguments looked swapped (field IDs are UUIDs); using '{}' as the field and '{}' as the task",
                t, field_id
            );
            (t, Some(field_id))
        }
        other => (field_id, other),
    }
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8usize, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(len, p)| p.len() == *len && p.chars().all(|c| c.is_ascii_hexdigit()))
}

pub async fn execute(command: FieldCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        FieldCommands::List {
            list,
            folder,
            space,
            workspace_level,
        } => {
            let path = if let Some(list_id) = list {
                format!("/v2/list/{}/field", list_id)
            } else if let Some(folder_id) = folder {
                format!("/v2/folder/{}/field", folder_id)
            } else if let Some(space_id) = space {
                format!("/v2/space/{}/field", space_id)
            } else if workspace_level {
                let ws_id = resolve_workspace(cli)?;
                format!("/v2/team/{}/field", ws_id)
            } else {
                return Err(CliError::ClientError {
                    message: "Specify --list, --folder, --space, or --workspace-level".into(),
                    status: 0,
                });
            };

            let resp = client.get(&path).await?;
            let fields = resp
                .get("fields")
                .and_then(|f| f.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&fields, FIELD_FIELDS, "id");
            Ok(())
        }
        FieldCommands::Set {
            task_id,
            field_id,
            value,
        } => {
            let (field_id, task_id) = maybe_unswap(field_id, task_id);
            let task = git::require_task(cli, task_id.as_deref(), true)?;
            // Try to parse value as JSON first, fallback to string
            let parsed_value: serde_json::Value =
                serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
            let body = serde_json::json!({ "value": parsed_value });
            let resp = client
                .post(&format!("/v2/task/{}/field/{}", task.id, field_id), &body)
                .await?;
            output.print_single(&resp, FIELD_FIELDS, "id");
            Ok(())
        }
        FieldCommands::Unset { task_id, field_id } => {
            let (field_id, task_id) = maybe_unswap(field_id, task_id);
            let task = git::require_task(cli, task_id.as_deref(), true)?;
            client
                .delete(&format!("/v2/task/{}/field/{}", task.id, field_id))
                .await?;
            output.print_message(&format!("Field {} cleared on task {}", field_id, task.raw));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_shape_is_recognized() {
        assert!(is_uuid("b955c4dc-b8a8-48d8-a0c6-b4200788a683"));
        assert!(is_uuid("B955C4DC-B8A8-48D8-A0C6-B4200788A683"));
    }

    #[test]
    fn task_id_shapes_are_not_uuids() {
        assert!(!is_uuid("86czkjbtq"));
        assert!(!is_uuid("PROJ-42"));
        assert!(!is_uuid("b955c4dc-b8a8-48d8-a0c6")); // too few segments
        assert!(!is_uuid("b955c4dc-b8a8-48d8-a0c6-b4200788a68z")); // non-hex
        assert!(!is_uuid("b955c4dcb8a848d8a0c6b4200788a683")); // no dashes
    }

    #[test]
    fn unswap_only_when_unambiguous() {
        // Swapped: task slot holds the UUID.
        let (f, t) = maybe_unswap(
            "86czkjbtq".into(),
            Some("b955c4dc-b8a8-48d8-a0c6-b4200788a683".into()),
        );
        assert_eq!(f, "b955c4dc-b8a8-48d8-a0c6-b4200788a683");
        assert_eq!(t.as_deref(), Some("86czkjbtq"));

        // Correct order: untouched.
        let (f, t) = maybe_unswap(
            "b955c4dc-b8a8-48d8-a0c6-b4200788a683".into(),
            Some("86czkjbtq".into()),
        );
        assert_eq!(f, "b955c4dc-b8a8-48d8-a0c6-b4200788a683");
        assert_eq!(t.as_deref(), Some("86czkjbtq"));

        // No task arg (git auto-detect path): untouched.
        let (f, t) = maybe_unswap("86czkjbtq".into(), None);
        assert_eq!(f, "86czkjbtq");
        assert_eq!(t, None);
    }
}
