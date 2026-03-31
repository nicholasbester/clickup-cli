use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum AuditLogCommands {
    /// Query audit logs (Enterprise only, v3)
    Query {
        /// Event type: AUTH, CUSTOM_FIELDS, HIERARCHY, USER, AGENT, OTHER
        #[arg(long)]
        r#type: String,
        /// Filter by user ID
        #[arg(long)]
        user_id: Option<String>,
        /// Start date (Unix timestamp in milliseconds)
        #[arg(long)]
        start_date: Option<i64>,
        /// End date (Unix timestamp in milliseconds)
        #[arg(long)]
        end_date: Option<i64>,
    },
}

const AUDIT_LOG_FIELDS: &[&str] = &["id", "type", "user_id", "date"];

pub async fn execute(command: AuditLogCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        AuditLogCommands::Query {
            r#type,
            user_id,
            start_date,
            end_date,
        } => {
            let team_id = resolve_workspace(cli)?;
            let mut body = serde_json::json!({ "type": r#type });
            if let Some(uid) = user_id {
                body["user_id"] = serde_json::Value::String(uid);
            }
            if start_date.is_some() || end_date.is_some() {
                let mut date_filter = serde_json::Map::new();
                if let Some(s) = start_date {
                    date_filter.insert("start_date".into(), serde_json::Value::Number(s.into()));
                }
                if let Some(e) = end_date {
                    date_filter.insert("end_date".into(), serde_json::Value::Number(e.into()));
                }
                body["date_filter"] = serde_json::Value::Object(date_filter);
            }
            let resp = client
                .post(
                    &format!("/v3/workspaces/{}/auditlogs", team_id),
                    &body,
                )
                .await?;

            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&resp).unwrap());
                return Ok(());
            }

            let logs = resp
                .get("data")
                .and_then(|d| d.as_array())
                .or_else(|| resp.get("audit_logs").and_then(|d| d.as_array()))
                .cloned()
                .unwrap_or_default();
            output.print_items(&logs, AUDIT_LOG_FIELDS, "id");
            Ok(())
        }
    }
}
