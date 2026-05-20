use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum AuditLogCommands {
    /// Query audit logs (Enterprise only, v3)
    Query {
        /// Required scope of the query. ClickUp's documented values: WORKSPACE, TEAMS, USERS.
        #[arg(long)]
        applicability: String,
        /// Filter by event type (e.g. AUTH, HIERARCHY, USER, CUSTOM_FIELDS, AGENT, OTHER)
        #[arg(long = "event-type")]
        event_type: Option<String>,
        /// Filter by event status (e.g. SUCCESS, FAILURE)
        #[arg(long = "event-status")]
        event_status: Option<String>,
        /// Filter by user ID (repeat for multiple)
        #[arg(long = "user-id")]
        user_id: Vec<String>,
        /// Filter by user email (repeat for multiple)
        #[arg(long = "user-email")]
        user_email: Vec<String>,
        /// Start time (Unix timestamp in milliseconds), maps to filter.startTime
        #[arg(long)]
        start_time: Option<i64>,
        /// End time (Unix timestamp in milliseconds), maps to filter.endTime
        #[arg(long)]
        end_time: Option<i64>,
        /// Max rows per page (pagination.pageRows)
        #[arg(long)]
        page_rows: Option<i64>,
        /// Cursor timestamp (pagination.pageTimestamp)
        #[arg(long)]
        page_timestamp: Option<i64>,
        /// Page direction (pagination.pageDirection): NEXT or PREVIOUS
        #[arg(long)]
        page_direction: Option<String>,
    },
}

const AUDIT_LOG_FIELDS: &[&str] = &["id", "eventType", "userId", "createdAt"];

pub async fn execute(command: AuditLogCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        AuditLogCommands::Query {
            applicability,
            event_type,
            event_status,
            user_id,
            user_email,
            start_time,
            end_time,
            page_rows,
            page_timestamp,
            page_direction,
        } => {
            let team_id = resolve_workspace(cli)?;
            let path = format!("/v3/workspaces/{}/auditlogs", team_id);

            let page_direction_owned = page_direction.clone();

            // The static portion of the pagination block (pageRows + direction)
            // is passed through unchanged on every iteration; pageTimestamp
            // is what the walker advances.
            let mut extra_pagination = serde_json::Map::new();
            if let Some(n) = page_rows {
                extra_pagination.insert("pageRows".into(), serde_json::Value::Number(n.into()));
            }
            if let Some(d) = page_direction_owned {
                extra_pagination.insert("pageDirection".into(), serde_json::Value::String(d));
            }

            let logs = crate::commands::pagination::walk_body(
                cli,
                &client,
                &path,
                &["data", "audit_logs"],
                || {
                    let mut body = serde_json::json!({ "applicability": applicability });
                    let mut filter = serde_json::Map::new();
                    if let Some(t) = &event_type {
                        filter.insert("eventType".into(), serde_json::json!(t));
                    }
                    if let Some(s) = &event_status {
                        filter.insert("eventStatus".into(), serde_json::json!(s));
                    }
                    if !user_id.is_empty() {
                        filter.insert(
                            "userId".into(),
                            serde_json::Value::Array(
                                user_id
                                    .iter()
                                    .cloned()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }
                    if !user_email.is_empty() {
                        filter.insert(
                            "userEmail".into(),
                            serde_json::Value::Array(
                                user_email
                                    .iter()
                                    .cloned()
                                    .map(serde_json::Value::String)
                                    .collect(),
                            ),
                        );
                    }
                    if let Some(s) = start_time {
                        filter.insert("startTime".into(), serde_json::json!(s));
                    }
                    if let Some(e) = end_time {
                        filter.insert("endTime".into(), serde_json::json!(e));
                    }
                    if !filter.is_empty() {
                        body["filter"] = serde_json::Value::Object(filter);
                    }
                    body
                },
                extra_pagination,
                page_timestamp,
                |item| {
                    for key in ["eventTime", "timestamp", "date"] {
                        if let Some(v) = item.get(key) {
                            if let Some(n) = v.as_i64() {
                                return Some(n);
                            }
                            if let Some(s) = v.as_str() {
                                if let Ok(n) = s.parse::<i64>() {
                                    return Some(n);
                                }
                            }
                        }
                    }
                    None
                },
            )
            .await?;

            if cli.output == "json" {
                println!("{}", serde_json::to_string_pretty(&logs).unwrap());
                return Ok(());
            }

            output.print_items(&logs, AUDIT_LOG_FIELDS, "id");
            Ok(())
        }
    }
}
