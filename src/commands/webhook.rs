use clap::Subcommand;
use crate::client::ClickUpClient;
use crate::commands::auth::resolve_token;
use crate::commands::workspace::resolve_workspace;
use crate::error::CliError;
use crate::output::OutputConfig;
use crate::Cli;

#[derive(Subcommand)]
pub enum WebhookCommands {
    /// List webhooks for the workspace
    List,
    /// Create a webhook
    Create {
        /// Endpoint URL
        #[arg(long)]
        endpoint: String,
        /// Event(s) to subscribe to (can be repeated)
        #[arg(long = "event")]
        events: Vec<String>,
        /// Scope to a specific space ID
        #[arg(long)]
        space: Option<String>,
        /// Scope to a specific folder ID
        #[arg(long)]
        folder: Option<String>,
        /// Scope to a specific list ID
        #[arg(long)]
        list: Option<String>,
        /// Scope to a specific task ID
        #[arg(long)]
        task: Option<String>,
    },
    /// Update a webhook
    Update {
        /// Webhook ID
        id: String,
        /// New endpoint URL
        #[arg(long)]
        endpoint: String,
        /// Event(s) to subscribe to (can be repeated)
        #[arg(long = "event")]
        events: Vec<String>,
        /// Webhook status: active or inactive
        #[arg(long, default_value = "active")]
        status: String,
    },
    /// Delete a webhook
    Delete {
        /// Webhook ID
        id: String,
    },
}

const WEBHOOK_FIELDS: &[&str] = &["id", "endpoint", "status", "events"];

pub async fn execute(command: WebhookCommands, cli: &Cli) -> Result<(), CliError> {
    let token = resolve_token(cli)?;
    let client = ClickUpClient::new(&token, cli.timeout)?;
    let output = OutputConfig::from_cli(&cli.output, &cli.fields, cli.no_header, cli.quiet);

    match command {
        WebhookCommands::List => {
            let ws_id = resolve_workspace(cli)?;
            let resp = client
                .get(&format!("/v2/team/{}/webhook", ws_id))
                .await?;
            let mut webhooks = resp
                .get("webhooks")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            if let Some(limit) = cli.limit {
                webhooks.truncate(limit);
            }
            output.print_items(&webhooks, WEBHOOK_FIELDS, "id");
            Ok(())
        }
        WebhookCommands::Create {
            endpoint,
            events,
            space,
            folder,
            list,
            task,
        } => {
            let ws_id = resolve_workspace(cli)?;
            let mut body = serde_json::json!({
                "endpoint": endpoint,
                "events": events,
            });
            if let Some(s) = space {
                body["space_id"] = serde_json::Value::String(s);
            }
            if let Some(f) = folder {
                body["folder_id"] = serde_json::Value::String(f);
            }
            if let Some(l) = list {
                body["list_id"] = serde_json::Value::String(l);
            }
            if let Some(t) = task {
                body["task_id"] = serde_json::Value::String(t);
            }
            let resp = client
                .post(&format!("/v2/team/{}/webhook", ws_id), &body)
                .await?;
            output.print_single(&resp, WEBHOOK_FIELDS, "id");
            Ok(())
        }
        WebhookCommands::Update {
            id,
            endpoint,
            events,
            status,
        } => {
            let body = serde_json::json!({
                "endpoint": endpoint,
                "events": events,
                "status": status,
            });
            let resp = client
                .put(&format!("/v2/webhook/{}", id), &body)
                .await?;
            output.print_single(&resp, WEBHOOK_FIELDS, "id");
            Ok(())
        }
        WebhookCommands::Delete { id } => {
            client.delete(&format!("/v2/webhook/{}", id)).await?;
            output.print_message(&format!("Webhook {} deleted", id));
            Ok(())
        }
    }
}
