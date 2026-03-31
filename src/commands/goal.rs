use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum GoalCommands {
    /// List goals in workspace
    List {
        /// Include completed goals
        #[arg(long)]
        include_completed: bool,
    },
    /// Get a goal by ID
    Get {
        /// Goal ID
        id: String,
    },
    /// Create a goal
    Create {
        /// Goal name
        #[arg(long)]
        name: String,
        /// Due date (Unix ms)
        #[arg(long)]
        due_date: String,
        /// Description
        #[arg(long)]
        description: String,
        /// Color hex (e.g. #32a852)
        #[arg(long)]
        color: Option<String>,
        /// Owner user ID
        #[arg(long)]
        owner: Option<String>,
    },
    /// Update a goal
    Update {
        /// Goal ID
        id: String,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New due date (Unix ms)
        #[arg(long)]
        due_date: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New color hex
        #[arg(long)]
        color: Option<String>,
        /// Add owner by user ID
        #[arg(long)]
        add_owner: Option<String>,
        /// Remove owner by user ID
        #[arg(long)]
        rem_owner: Option<String>,
    },
    /// Delete a goal
    Delete {
        /// Goal ID
        id: String,
    },
    /// Add a key result to a goal
    AddKr {
        /// Goal ID
        goal_id: String,
        /// Key result name
        #[arg(long)]
        name: String,
        /// Type: number, currency, boolean, percentage, automatic
        #[arg(long, name = "type")]
        kr_type: String,
        /// Starting step value
        #[arg(long)]
        steps_start: i64,
        /// Target step value
        #[arg(long)]
        steps_end: i64,
        /// Unit label (e.g. "tasks")
        #[arg(long)]
        unit: Option<String>,
        /// Owner user ID
        #[arg(long)]
        owner: Option<String>,
    },
    /// Update a key result
    UpdateKr {
        /// Key result ID
        kr_id: String,
        /// Current step value
        #[arg(long)]
        steps_current: i64,
        /// Note
        #[arg(long)]
        note: Option<String>,
    },
    /// Delete a key result
    DeleteKr {
        /// Key result ID
        kr_id: String,
    },
}

const GOAL_FIELDS: &[&str] = &["id", "name", "percent_completed", "due_date"];

pub async fn execute(command: GoalCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        GoalCommands::List { include_completed } => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!(
                    "/v2/team/{}/goal?include_completed={}",
                    ws_id, include_completed
                ))
                .await?;
            let goals = resp
                .get("goals")
                .and_then(|g| g.as_array())
                .cloned()
                .unwrap_or_default();
            output.print_items(&goals, GOAL_FIELDS, "id");
            Ok(())
        }
        GoalCommands::Get { id } => {
            let resp = client.get(&format!("/v2/goal/{}", id)).await?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            output.print_single(&goal, GOAL_FIELDS, "id");
            Ok(())
        }
        GoalCommands::Create {
            name,
            due_date,
            description,
            color,
            owner,
        } => {
            let ws_id = resolve_workspace(cli)?;
            let mut body = serde_json::json!({
                "name": name,
                "due_date": due_date,
                "description": description,
            });
            if let Some(c) = color {
                body["color"] = serde_json::Value::String(c);
            }
            if let Some(o) = owner {
                body["owners"] = serde_json::json!([o]);
            }
            let resp = client
                .post(&format!("/v2/team/{}/goal", ws_id), &body)
                .await?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            output.print_single(&goal, GOAL_FIELDS, "id");
            Ok(())
        }
        GoalCommands::Update {
            id,
            name,
            due_date,
            description,
            color,
            add_owner,
            rem_owner,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".into(), serde_json::Value::String(n));
            }
            if let Some(d) = due_date {
                body.insert("due_date".into(), serde_json::Value::String(d));
            }
            if let Some(d) = description {
                body.insert("description".into(), serde_json::Value::String(d));
            }
            if let Some(c) = color {
                body.insert("color".into(), serde_json::Value::String(c));
            }
            if let Some(o) = add_owner {
                body.insert("add_owners".into(), serde_json::json!([o]));
            }
            if let Some(o) = rem_owner {
                body.insert("rem_owners".into(), serde_json::json!([o]));
            }
            let resp = client
                .put(&format!("/v2/goal/{}", id), &serde_json::Value::Object(body))
                .await?;
            let goal = resp.get("goal").cloned().unwrap_or(resp);
            output.print_single(&goal, GOAL_FIELDS, "id");
            Ok(())
        }
        GoalCommands::Delete { id } => {
            client.delete(&format!("/v2/goal/{}", id)).await?;
            output.print_message(&format!("Goal {} deleted", id));
            Ok(())
        }
        GoalCommands::AddKr {
            goal_id,
            name,
            kr_type,
            steps_start,
            steps_end,
            unit,
            owner,
        } => {
            let mut body = serde_json::json!({
                "name": name,
                "type": kr_type,
                "steps_start": steps_start,
                "steps_end": steps_end,
            });
            if let Some(u) = unit {
                body["unit"] = serde_json::Value::String(u);
            }
            if let Some(o) = owner {
                body["owners"] = serde_json::json!([o]);
            }
            let resp = client
                .post(&format!("/v2/goal/{}/key_result", goal_id), &body)
                .await?;
            let kr = resp.get("key_result").cloned().unwrap_or(resp);
            output.print_single(&kr, &["id", "name", "type", "steps_start", "steps_end"], "id");
            Ok(())
        }
        GoalCommands::UpdateKr {
            kr_id,
            steps_current,
            note,
        } => {
            let mut body = serde_json::json!({ "steps_current": steps_current });
            if let Some(n) = note {
                body["note"] = serde_json::Value::String(n);
            }
            let resp = client
                .put(&format!("/v2/key_result/{}", kr_id), &body)
                .await?;
            let kr = resp.get("key_result").cloned().unwrap_or(resp);
            output.print_single(&kr, &["id", "name", "steps_current", "steps_end"], "id");
            Ok(())
        }
        GoalCommands::DeleteKr { kr_id } => {
            client.delete(&format!("/v2/key_result/{}", kr_id)).await?;
            output.print_message(&format!("Key result {} deleted", kr_id));
            Ok(())
        }
    }
}
